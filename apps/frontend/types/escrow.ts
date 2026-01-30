export interface IEscrow {
  id: string;
  title: string;
  description: string;
  amount: string;
  asset: string;
  creatorAddress: string;
  counterpartyAddress: string;
  deadline: string; // ISO date string
  status: 'created' | 'funded' | 'confirmed' | 'released' | 'completed' | 'cancelled' | 'disputed';
  createdAt: string; // ISO date string
  updatedAt: string; // ISO date string
  milestones?: Array<{
    id: string;
    title: string;
    amount: string;
    status: 'pending' | 'released';
  }>;
}

export interface IEscrowResponse {
  escrows: IEscrow[];
  hasNextPage: boolean;
  totalPages?: number;
  totalCount?: number;
}

export interface IEscrowFilters {
  status?: 'all' | 'active' | 'pending' | 'completed' | 'disputed';
  search?: string;
  sortBy?: 'date' | 'amount' | 'deadline';
  sortOrder?: 'asc' | 'desc';
  page?: number;
  limit?: number;
}