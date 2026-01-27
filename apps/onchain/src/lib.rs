#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

// Milestone status tracking
#[contracttype]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MilestoneStatus {
    Pending,
    Released,
    Disputed,
}

// Individual milestone in an escrow
#[contracttype]
#[derive(Clone, Debug)]
pub struct Milestone {
    pub amount: i128,
    pub status: MilestoneStatus,
    pub description: Symbol,
}

// Overall escrow status
#[contracttype]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EscrowStatus {
    Active,
    Completed,
    Cancelled,
    Disputed,
    Resolved,
}

// Dispute resolution outcome
#[contracttype]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resolution {
    None,
    Depositor,
    Recipient,
}

// Main escrow structure
#[contracttype]
#[derive(Clone, Debug)]
pub struct Escrow {
    pub depositor: Address,
    pub recipient: Address,
    pub total_amount: i128,
    pub total_released: i128,
    pub milestones: Vec<Milestone>,
    pub status: EscrowStatus,
    pub resolution: Resolution,
}

// Contract error types
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    EscrowNotFound = 1,
    EscrowAlreadyExists = 2,
    MilestoneNotFound = 3,
    MilestoneAlreadyReleased = 4,
    UnauthorizedAccess = 5,
    InvalidMilestoneAmount = 6,
    TotalAmountMismatch = 7,
    InsufficientBalance = 8,
    EscrowNotActive = 9,
    VectorTooLarge = 10,
    AdminNotInitialized = 11,
    AlreadyInitialized = 12,
    InvalidEscrowStatus = 13,
    AlreadyInDispute = 14,
    InvalidWinner = 15,
}

#[contract]
pub struct VaultixEscrow;

#[contractimpl]
impl VaultixEscrow {
    /// Initializes the contract with an admin address responsible for dispute resolution.
    pub fn init(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().persistent().has(&admin_storage_key()) {
            return Err(Error::AlreadyInitialized);
        }

        admin.require_auth();
        env.storage().persistent().set(&admin_storage_key(), &admin);
        Ok(())
    }

    /// Creates a new escrow with milestone-based payment releases.
    ///
    /// # Arguments
    /// * `escrow_id` - Unique identifier for the escrow
    /// * `depositor` - Address funding the escrow
    /// * `recipient` - Address receiving milestone payments
    /// * `milestones` - Vector of milestones defining payment schedule
    ///
    /// # Errors
    /// * `EscrowAlreadyExists` - If escrow_id is already in use
    /// * `VectorTooLarge` - If more than 20 milestones provided
    /// * `InvalidMilestoneAmount` - If any milestone amount is zero or negative
    pub fn create_escrow(
        env: Env,
        escrow_id: u64,
        depositor: Address,
        recipient: Address,
        milestones: Vec<Milestone>,
    ) -> Result<(), Error> {
        // Authenticate the depositor
        depositor.require_auth();

        // Check if escrow already exists
        let storage_key = get_storage_key(escrow_id);
        if env.storage().persistent().has(&storage_key) {
            return Err(Error::EscrowAlreadyExists);
        }

        // Validate milestones and calculate total
        let total_amount = validate_milestones(&milestones)?;

        // Initialize all milestones to Pending status
        let mut initialized_milestones = Vec::new(&env);
        for milestone in milestones.iter() {
            let mut m = milestone.clone();
            m.status = MilestoneStatus::Pending;
            initialized_milestones.push_back(m);
        }

        // Create the escrow
        let escrow = Escrow {
            depositor: depositor.clone(),
            recipient,
            total_amount,
            total_released: 0,
            milestones: initialized_milestones,
            status: EscrowStatus::Active,
            resolution: Resolution::None,
        };

        // Save to persistent storage
        env.storage().persistent().set(&storage_key, &escrow);

        Ok(())
    }

    /// Releases a specific milestone payment to the recipient.
    ///
    /// # Arguments
    /// * `escrow_id` - Identifier of the escrow
    /// * `milestone_index` - Index of the milestone to release
    ///
    /// # Errors
    /// * `EscrowNotFound` - If escrow doesn't exist
    /// * `UnauthorizedAccess` - If caller is not the depositor
    /// * `EscrowNotActive` - If escrow is completed or cancelled
    /// * `MilestoneNotFound` - If index is out of bounds
    /// * `MilestoneAlreadyReleased` - If milestone was already released
    pub fn release_milestone(env: Env, escrow_id: u64, milestone_index: u32) -> Result<(), Error> {
        let storage_key = get_storage_key(escrow_id);

        // Load escrow from storage
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&storage_key)
            .ok_or(Error::EscrowNotFound)?;

        // Verify authorization
        escrow.depositor.require_auth();

        // Check escrow is active
        if escrow.status != EscrowStatus::Active {
            return Err(Error::EscrowNotActive);
        }

        // Verify milestone index is valid
        if milestone_index >= escrow.milestones.len() {
            return Err(Error::MilestoneNotFound);
        }

        // Get the milestone
        let mut milestone = escrow
            .milestones
            .get(milestone_index)
            .ok_or(Error::MilestoneNotFound)?;

        // Check if already released
        if milestone.status == MilestoneStatus::Released {
            return Err(Error::MilestoneAlreadyReleased);
        }

        // Update milestone status
        milestone.status = MilestoneStatus::Released;
        escrow.milestones.set(milestone_index, milestone.clone());

        // Update total released with overflow protection
        escrow.total_released = escrow
            .total_released
            .checked_add(milestone.amount)
            .ok_or(Error::InvalidMilestoneAmount)?;

        // Save updated escrow
        env.storage().persistent().set(&storage_key, &escrow);

        Ok(())
    }

    /// Raises a dispute on an active escrow. Either party (depositor or recipient) may invoke this.
    pub fn raise_dispute(env: Env, escrow_id: u64, caller: Address) -> Result<(), Error> {
        let storage_key = get_storage_key(escrow_id);

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&storage_key)
            .ok_or(Error::EscrowNotFound)?;

        if caller != escrow.depositor && caller != escrow.recipient {
            return Err(Error::UnauthorizedAccess);
        }
        caller.require_auth();

        if escrow.status == EscrowStatus::Disputed {
            return Err(Error::AlreadyInDispute);
        }
        if escrow.status != EscrowStatus::Active {
            return Err(Error::InvalidEscrowStatus);
        }

        // Mark pending milestones as disputed to freeze further releases.
        let mut updated_milestones = Vec::new(&env);
        for milestone in escrow.milestones.iter() {
            let mut m = milestone.clone();
            if m.status == MilestoneStatus::Pending {
                m.status = MilestoneStatus::Disputed;
            }
            updated_milestones.push_back(m);
        }

        escrow.milestones = updated_milestones;
        escrow.status = EscrowStatus::Disputed;
        escrow.resolution = Resolution::None;
        env.storage().persistent().set(&storage_key, &escrow);

        Ok(())
    }

    /// Resolves an active dispute by directing funds to the chosen party. Only the admin may call this.
    pub fn resolve_dispute(env: Env, escrow_id: u64, winner: Address) -> Result<(), Error> {
        let admin = get_admin(&env)?;
        admin.require_auth();

        let storage_key = get_storage_key(escrow_id);

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&storage_key)
            .ok_or(Error::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Disputed {
            return Err(Error::InvalidEscrowStatus);
        }

        // Winner must be one of the parties
        if winner != escrow.depositor && winner != escrow.recipient {
            return Err(Error::InvalidWinner);
        }

        // Release or refund remaining funds based on winner
        if winner == escrow.recipient {
            // Force release of all pending/disputed milestones
            let mut updated_milestones = Vec::new(&env);
            for milestone in escrow.milestones.iter() {
                let mut m = milestone.clone();
                if m.status != MilestoneStatus::Released {
                    m.status = MilestoneStatus::Released;
                }
                updated_milestones.push_back(m);
            }
            escrow.milestones = updated_milestones;
            escrow.total_released = escrow.total_amount;
            escrow.resolution = Resolution::Recipient;
        } else {
            // Refund remaining funds to depositor; keep already released milestones as-is
            let mut updated_milestones = Vec::new(&env);
            for milestone in escrow.milestones.iter() {
                let mut m = milestone.clone();
                if m.status == MilestoneStatus::Pending || m.status == MilestoneStatus::Disputed {
                    m.status = MilestoneStatus::Disputed;
                }
                updated_milestones.push_back(m);
            }
            escrow.milestones = updated_milestones;
            escrow.resolution = Resolution::Depositor;
        }

        escrow.status = EscrowStatus::Resolved;
        env.storage().persistent().set(&storage_key, &escrow);

        Ok(())
    }

    /// Retrieves escrow details.
    ///
    /// # Arguments
    /// * `escrow_id` - Identifier of the escrow
    ///
    /// # Returns
    /// The complete Escrow struct
    ///
    /// # Errors
    /// * `EscrowNotFound` - If escrow doesn't exist
    pub fn get_escrow(env: Env, escrow_id: u64) -> Result<Escrow, Error> {
        let storage_key = get_storage_key(escrow_id);
        env.storage()
            .persistent()
            .get(&storage_key)
            .ok_or(Error::EscrowNotFound)
    }

    /// Cancels an escrow before any milestones are released.
    ///
    /// # Arguments
    /// * `escrow_id` - Identifier of the escrow
    ///
    /// # Errors
    /// * `EscrowNotFound` - If escrow doesn't exist
    /// * `UnauthorizedAccess` - If caller is not the depositor
    /// * `MilestoneAlreadyReleased` - If any milestone has been released
    pub fn cancel_escrow(env: Env, escrow_id: u64) -> Result<(), Error> {
        let storage_key = get_storage_key(escrow_id);

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&storage_key)
            .ok_or(Error::EscrowNotFound)?;

        // Verify authorization
        escrow.depositor.require_auth();

        if escrow.status != EscrowStatus::Active {
            return Err(Error::InvalidEscrowStatus);
        }

        // Verify no milestones have been released
        if escrow.total_released > 0 {
            return Err(Error::MilestoneAlreadyReleased);
        }

        // Update status
        escrow.status = EscrowStatus::Cancelled;
        env.storage().persistent().set(&storage_key, &escrow);

        Ok(())
    }

    /// Marks an escrow as completed after all milestones are released.
    ///
    /// # Arguments
    /// * `escrow_id` - Identifier of the escrow
    ///
    /// # Errors
    /// * `EscrowNotFound` - If escrow doesn't exist
    /// * `UnauthorizedAccess` - If caller is not the depositor
    /// * `EscrowNotActive` - If not all milestones are released
    pub fn complete_escrow(env: Env, escrow_id: u64) -> Result<(), Error> {
        let storage_key = get_storage_key(escrow_id);

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&storage_key)
            .ok_or(Error::EscrowNotFound)?;

        // Verify authorization
        escrow.depositor.require_auth();

        if escrow.status != EscrowStatus::Active {
            return Err(Error::InvalidEscrowStatus);
        }

        // Verify all milestones are released
        if !verify_all_released(&escrow.milestones) {
            return Err(Error::EscrowNotActive);
        }

        // Update status
        escrow.status = EscrowStatus::Completed;
        env.storage().persistent().set(&storage_key, &escrow);

        Ok(())
    }
}

// Helper function to generate storage key
fn get_storage_key(escrow_id: u64) -> (Symbol, u64) {
    (symbol_short!("escrow"), escrow_id)
}

fn admin_storage_key() -> Symbol {
    symbol_short!("admin")
}

fn get_admin(env: &Env) -> Result<Address, Error> {
    env.storage()
        .persistent()
        .get(&admin_storage_key())
        .ok_or(Error::AdminNotInitialized)
}

// Validates milestone vector and returns total amount
fn validate_milestones(milestones: &Vec<Milestone>) -> Result<i128, Error> {
    // Check vector size to prevent gas issues
    if milestones.len() > 20 {
        return Err(Error::VectorTooLarge);
    }

    let mut total: i128 = 0;

    // Validate each milestone and calculate total
    for milestone in milestones.iter() {
        if milestone.amount <= 0 {
            return Err(Error::InvalidMilestoneAmount);
        }

        total = total
            .checked_add(milestone.amount)
            .ok_or(Error::InvalidMilestoneAmount)?;
    }

    Ok(total)
}

// Checks if all milestones have been released
fn verify_all_released(milestones: &Vec<Milestone>) -> bool {
    for milestone in milestones.iter() {
        if milestone.status != MilestoneStatus::Released {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod test;
