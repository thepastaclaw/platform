const {
  server: {
    error: {
      InvalidArgumentGrpcError,
      AlreadyExistsGrpcError,
      UnavailableGrpcError,
      ResourceExhaustedGrpcError,
      InternalGrpcError,
    },
  },
} = require('@dashevo/grpc-common');

const {
  v0: {
    BroadcastStateTransitionResponse,
  },
} = require('@dashevo/dapi-grpc');

const { default: loadWasmDpp, DashPlatformProtocol } = require('@dashevo/wasm-dpp');
const getDataContractFixture = require('@dashevo/wasm-dpp/lib/test/fixtures/getDataContractFixture');

const GrpcErrorCodes = require('@dashevo/grpc-common/lib/server/error/GrpcErrorCodes');
const NotFoundGrpcError = require('@dashevo/grpc-common/lib/server/error/NotFoundGrpcError');
const cbor = require('cbor');
const crypto = require('crypto');
const GrpcCallMock = require('../../../../../lib/test/mock/GrpcCallMock');
const RPCError = require('../../../../../lib/rpcServer/RPCError');

const broadcastStateTransitionHandlerFactory = require(
  '../../../../../lib/grpcServer/handlers/platform/broadcastStateTransitionHandlerFactory',
);

describe('broadcastStateTransitionHandlerFactory', () => {
  let call;
  let rpcClientMock;
  let broadcastStateTransitionHandler;
  let response;
  let stateTransitionFixture;
  let log;
  let code;
  let createGrpcErrorFromDriveResponseMock;
  let requestTenderRpcMock;

  before(async () => {
    await loadWasmDpp();
  });

  beforeEach(async function beforeEach() {
    const dpp = new DashPlatformProtocol(null, null);

    const dataContractFixture = await getDataContractFixture();
    stateTransitionFixture = dpp.dataContract.createDataContractCreateTransition(
      dataContractFixture,
    );

    call = new GrpcCallMock(this.sinon, {
      getStateTransition: this.sinon.stub().returns(stateTransitionFixture.toBuffer()),
    });

    log = JSON.stringify({
      error: {
        message: 'some message',
        data: {
          error: 'some data',
        },
      },
    });

    code = 0;

    response = {
      id: '',
      jsonrpc: '2.0',
      error: '',
      result: {
        check_tx: { code, log },
        deliver_tx: { code, log },
        hash:
        'B762539A7C17C33A65C46727BFCF2C701390E6AD7DE5190B6CC1CF843CA7E262',
        height: '24',
        code,
      },
    };

    rpcClientMock = {
      request: this.sinon.stub().resolves(response),
    };

    requestTenderRpcMock = this.sinon.stub();

    createGrpcErrorFromDriveResponseMock = this.sinon.stub();

    broadcastStateTransitionHandler = broadcastStateTransitionHandlerFactory(
      rpcClientMock,
      createGrpcErrorFromDriveResponseMock,
      requestTenderRpcMock,
    );
  });

  afterEach(function afterEach() {
    this.sinon.restore();
  });

  it('should throw an InvalidArgumentGrpcError if stateTransition is not specified', async () => {
    call.request.getStateTransition.returns(null);

    try {
      await broadcastStateTransitionHandler(call);

      expect.fail('InvalidArgumentGrpcError was not thrown');
    } catch (e) {
      expect(e).to.be.an.instanceOf(InvalidArgumentGrpcError);
      expect(e.getMessage()).to.equal('State Transition is not specified');
      expect(rpcClientMock.request).to.not.be.called();
    }
  });

  it('should return valid result', async () => {
    const result = await broadcastStateTransitionHandler(call);

    const tx = stateTransitionFixture.toBuffer().toString('base64');

    expect(result).to.be.an.instanceOf(BroadcastStateTransitionResponse);
    expect(rpcClientMock.request).to.be.calledOnceWith('broadcast_tx', { tx });
  });

  it('should throw a UnavailableGrpcError if tenderdash hands up', async () => {
    const error = new Error('socket hang up');
    rpcClientMock.request.throws(error);

    try {
      await broadcastStateTransitionHandler(call);

      expect.fail('should throw UnavailableGrpcError');
    } catch (e) {
      expect(e).to.be.an.instanceOf(UnavailableGrpcError);
      expect(e.getMessage()).to.equal('Tenderdash is not available');
    }
  });

  it('should throw a UnavailableGrpcError if broadcast confirmation not received', async () => {
    response.error = {
      code: -32603,
      message: 'Internal error',
      data: 'broadcast confirmation not received: heya',
    };

    try {
      await broadcastStateTransitionHandler(call);

      expect.fail('should throw UnavailableGrpcError');
    } catch (e) {
      expect(e).to.be.an.instanceOf(UnavailableGrpcError);
      expect(e.getMessage()).to.equal(response.error.data);
    }
  });

  it('should throw an InvalidArgumentGrpcError if state transition size is too big', async () => {
    response.error = {
      code: -32603,
      message: 'Internal error',
      data: 'Tx too large. La la la',
    };

    try {
      await broadcastStateTransitionHandler(call);

      expect.fail('should throw UnavailableGrpcError');
    } catch (e) {
      expect(e).to.be.an.instanceOf(InvalidArgumentGrpcError);
      expect(e.getMessage()).to.equal('state transition is too large. La la la');
    }
  });

  it('should throw a ResourceExhaustedGrpcError if mempool is full', async () => {
    response.error = {
      code: -32603,
      message: 'Internal error',
      data: 'mempool is full: heya',
    };

    try {
      await broadcastStateTransitionHandler(call);

      expect.fail('should throw UnavailableGrpcError');
    } catch (e) {
      expect(e).to.be.an.instanceOf(ResourceExhaustedGrpcError);
      expect(e.getMessage()).to.equal(response.error.data);
    }
  });

  it('should throw AlreadyExistsGrpcError if transaction in mempool', async () => {
    response.error = {
      code: -32603,
      message: 'Internal error',
      data: 'tx already exists in cache',
    };

    const stBytes = stateTransitionFixture.toBuffer();
    const stHashBase64 = crypto.createHash('sha256').update(stBytes).digest()
      .toString('base64');
    const stHashHex = `0x${crypto.createHash('sha256').update(stBytes).digest('hex')}`;

    requestTenderRpcMock.withArgs('unconfirmed_tx').resolves({
      tx: stBytes.toString('base64'),
    });

    try {
      await broadcastStateTransitionHandler(call);

      expect.fail('should throw AlreadyExistsGrpcError');
    } catch (e) {
      expect(e).to.be.an.instanceOf(AlreadyExistsGrpcError);
      expect(e.getMessage()).to.equal('state transition already in mempool');

      // The handler must look up the specific ST by base64-encoded sha256
      // hash, not page through the mempool or use a 0x-prefixed hex hash.
      expect(requestTenderRpcMock).to.be.calledWith('unconfirmed_tx', { hash: stHashBase64 });
      expect(requestTenderRpcMock).to.not.be.calledWith('unconfirmed_tx', { hash: stHashHex });
      expect(requestTenderRpcMock).to.not.be.calledWith('unconfirmed_txs');
    }
  });

  it('should fall through to chain lookup when unconfirmed_tx is not found', async () => {
    response.error = {
      code: -32603,
      message: 'Internal error',
      data: 'tx already exists in cache',
    };

    const stBytes = stateTransitionFixture.toBuffer();
    const stHashBase64 = crypto.createHash('sha256').update(stBytes).digest()
      .toString('base64');
    const stHashHex = `0x${crypto.createHash('sha256').update(stBytes).digest('hex')}`;

    requestTenderRpcMock.withArgs('unconfirmed_tx').rejects(
      new RPCError(-32603, 'Internal error', 'tx (...) not found'),
    );

    requestTenderRpcMock.withArgs('tx').resolves({
      tx_result: { },
    });

    try {
      await broadcastStateTransitionHandler(call);

      expect.fail('should throw AlreadyExistsGrpcError');
    } catch (e) {
      expect(e).to.be.an.instanceOf(AlreadyExistsGrpcError);
      expect(e.getMessage()).to.equal('state transition already in chain');

      expect(requestTenderRpcMock).to.be.calledWith('unconfirmed_tx', { hash: stHashBase64 });
      expect(requestTenderRpcMock).to.not.be.calledWith('unconfirmed_tx', { hash: stHashHex });
      expect(requestTenderRpcMock).to.be.calledWith('tx', { hash: stHashBase64 });
      expect(requestTenderRpcMock).to.not.be.calledWith('unconfirmed_txs');
    }
  });

  it('should re-throw unexpected errors from unconfirmed_tx', async () => {
    response.error = {
      code: -32603,
      message: 'Internal error',
      data: 'tx already exists in cache',
    };

    const unexpectedError = new RPCError(-32603, 'Internal error', 'something went terribly wrong');

    requestTenderRpcMock.withArgs('unconfirmed_tx').rejects(unexpectedError);

    try {
      await broadcastStateTransitionHandler(call);

      expect.fail('should re-throw the unexpected error');
    } catch (e) {
      expect(e).to.equal(unexpectedError);
      expect(requestTenderRpcMock).to.not.be.calledWith('tx');
      expect(requestTenderRpcMock).to.not.be.calledWith('check_tx');
      expect(requestTenderRpcMock).to.not.be.calledWith('unconfirmed_txs');
    }
  });

  it('should throw AlreadyExistsGrpcError if transaction in chain', async () => {
    response.error = {
      code: -32603,
      message: 'Internal error',
      data: 'tx already exists in cache',
    };

    requestTenderRpcMock.withArgs('unconfirmed_tx').rejects(
      new RPCError(-32603, 'Internal error', 'tx not found'),
    );

    requestTenderRpcMock.withArgs('tx').resolves({
      tx_result: { },
    });

    try {
      await broadcastStateTransitionHandler(call);

      expect.fail('should throw AlreadyExistsGrpcError');
    } catch (e) {
      expect(e).to.be.an.instanceOf(AlreadyExistsGrpcError);
      expect(e.getMessage()).to.equal('state transition already in chain');
      expect(requestTenderRpcMock).to.not.be.calledWith('unconfirmed_txs');
    }
  });

  it('should throw consensus result for invalid transition in cache', async () => {
    response.error = {
      code: -32603,
      message: 'Internal error',
      data: 'tx already exists in cache',
    };

    requestTenderRpcMock.withArgs('unconfirmed_tx').rejects(
      new RPCError(-32603, 'Internal error', 'tx not found'),
    );

    requestTenderRpcMock.withArgs('check_tx').resolves({
      code: 1,
      info: 'some info',
    });

    const error = new Error('some error');

    createGrpcErrorFromDriveResponseMock.resolves(error);

    try {
      await broadcastStateTransitionHandler(call);

      expect.fail('should throw consensus error');
    } catch (e) {
      expect(e).to.equal(error);
      expect(requestTenderRpcMock).to.not.be.calledWith('unconfirmed_txs');
    }
  });

  it('should throw internal error for transition in cache that passing check tx', async () => {
    response.error = {
      code: -32603,
      message: 'Internal error',
      data: 'tx already exists in cache',
    };

    requestTenderRpcMock.withArgs('unconfirmed_tx').rejects(
      new RPCError(-32603, 'Internal error', 'tx not found'),
    );

    requestTenderRpcMock.withArgs('check_tx').resolves({
      code: 0,
    });

    try {
      await broadcastStateTransitionHandler(call);

      expect.fail('should throw InternalError');
    } catch (e) {
      expect(e).to.be.an.instanceOf(InternalGrpcError);
      expect(e.getMessage()).to.equal('Internal error');
      expect(requestTenderRpcMock).to.not.be.calledWith('unconfirmed_txs');
    }
  });

  it('should throw a gRPC error based on drive\'s response', async () => {
    const message = 'not found';
    const metadata = {
      data: 'some data',
    };

    createGrpcErrorFromDriveResponseMock.returns(
      new NotFoundGrpcError(message, metadata),
    );

    response.result.code = GrpcErrorCodes.NOT_FOUND;
    response.result.info = cbor.encode({ message, metadata }).toString('base64');

    try {
      await broadcastStateTransitionHandler(call);

      expect.fail('should throw AlreadyExistsGrpcError');
    } catch (e) {
      expect(e).to.be.an.instanceOf(NotFoundGrpcError);
      expect(e.getMessage()).to.equal(message);
      expect(e.getRawMetadata()).to.deep.equal(metadata);
      expect(e.getCode()).to.equal(response.result.code);
      expect(createGrpcErrorFromDriveResponseMock).to.be.calledWithExactly(
        response.result.code,
        response.result.info,
      );
    }
  });

  it('should throw an error if transaction broadcast returns unknown error', async () => {
    const error = { code: -1, message: "Something didn't work", data: 'Some data' };

    response.error = error;

    try {
      await broadcastStateTransitionHandler(call);

      expect.fail('should throw an error');
    } catch (e) {
      expect(e.message).to.equal(error.message);
      expect(e.data).to.equal(error.data);
      expect(e.code).to.equal(error.code);
    }
  });
});
