import { Test, TestingModule } from '@nestjs/testing';
import { INestApplication } from '@nestjs/common';
import supertest from 'supertest';
import { AppModule } from '../src/app.module';
import { TypeOrmModule } from '@nestjs/typeorm';
import { User } from '../src/modules/user/entities/user.entity';
import { RefreshToken } from '../src/modules/user/entities/refresh-token.entity';
import * as StellarSdk from 'stellar-sdk';

describe('Authentication (e2e)', () => {
  let app: INestApplication;
  let testKeypair: StellarSdk.Keypair;
  let testWalletAddress: string;

  beforeAll(async () => {
    // Generate a test keypair
    testKeypair = StellarSdk.Keypair.random();
    testWalletAddress = testKeypair.publicKey();

    const moduleFixture: TestingModule = await Test.createTestingModule({
      imports: [
        AppModule,
        TypeOrmModule.forRoot({
          type: 'sqlite',
          database: ':memory:',
          entities: [User, RefreshToken],
          synchronize: true,
        }),
      ],
    }).compile();

    app = moduleFixture.createNestApplication();
    await app.init();
  });

  afterAll(async () => {
    await app.close();
  });

  describe('/auth/challenge (POST)', () => {
    it('should return a unique nonce for a wallet address', async () => {
      const response = await supertest(app.getHttpServer())
        .post('/auth/challenge')
        .send({ walletAddress: testWalletAddress })
        .expect(200);

      expect(response.body).toHaveProperty('nonce');
      expect(response.body).toHaveProperty('message');
      expect(response.body.message).toContain(response.body.nonce);
      expect(response.body.nonce).toMatch(/^[a-f0-9]{32}$/);
    });

    it('should return 400 for invalid wallet address', async () => {
      await supertest(app.getHttpServer())
        .post('/auth/challenge')
        .send({ walletAddress: 'invalid-address' })
        .expect(400);
    });
  });

  describe('/auth/verify (POST)', () => {
    it('should verify a valid signature and return tokens', async () => {
      // First get a challenge
      const challengeResponse = await supertest(app.getHttpServer())
        .post('/auth/challenge')
        .send({ walletAddress: testWalletAddress })
        .expect(200);

      const message = challengeResponse.body.message;
      const signature = testKeypair.sign(message).toString('hex');

      const response = await supertest(app.getHttpServer())
        .post('/auth/verify')
        .send({
          walletAddress: testWalletAddress,
          signature: signature,
          publicKey: testWalletAddress,
        })
        .expect(200);

      expect(response.body).toHaveProperty('accessToken');
      expect(response.body).toHaveProperty('refreshToken');
      expect(response.body.accessToken).toBeDefined();
      expect(response.body.refreshToken).toBeDefined();
    });

    it('should return 401 for invalid signature', async () => {
      await supertest(app.getHttpServer())
        .post('/auth/verify')
        .send({
          walletAddress: testWalletAddress,
          signature: 'invalid-signature',
          publicKey: testWalletAddress,
        })
        .expect(401);
    });
  });

  describe('/auth/me (GET)', () => {
    let accessToken: string;

    beforeEach(async () => {
      // Get a valid access token
      const challengeResponse = await supertest(app.getHttpServer())
        .post('/auth/challenge')
        .send({ walletAddress: testWalletAddress })
        .expect(200);

      const message = challengeResponse.body.message;
      const signature = testKeypair.sign(message).toString('hex');

      const verifyResponse = await supertest(app.getHttpServer())
        .post('/auth/verify')
        .send({
          walletAddress: testWalletAddress,
          signature: signature,
          publicKey: testWalletAddress,
        })
        .expect(200);

      accessToken = verifyResponse.body.accessToken;
    });

    it('should return current user with valid token', async () => {
      const response = await supertest(app.getHttpServer())
        .get('/auth/me')
        .set('Authorization', `Bearer ${accessToken}`)
        .expect(200);

      expect(response.body).toHaveProperty('id');
      expect(response.body).toHaveProperty('walletAddress');
      expect(response.body).toHaveProperty('isActive');
      expect(response.body).toHaveProperty('createdAt');
      expect(response.body.walletAddress).toBe(testWalletAddress);
      expect(response.body.isActive).toBe(true);
    });

    it('should return 401 without token', async () => {
      await supertest(app.getHttpServer())
        .get('/auth/me')
        .expect(401);
    });

    it('should return 401 with invalid token', async () => {
      await supertest(app.getHttpServer())
        .get('/auth/me')
        .set('Authorization', 'Bearer invalid-token')
        .expect(401);
    });
  });
});
