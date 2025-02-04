/* eslint-disable */
import * as _m0 from "protobufjs/minimal";
import { Empty } from "../../../google/protobuf/empty";

export const protobufPackage = "quary.service.v1";

/** TestResult is the result of a test serializable so it can be shared */
export interface TestResult {
  testName: string;
  query: string;
  testResult?: { $case: "passed"; passed: Passed } | { $case: "failed"; failed: Failed } | undefined;
}

export interface InferredChain {
  inferredChain: string[];
}

export interface InferredChainWithOperation {
  inferredChain: string[];
  operation: string;
}

export interface Passed {
  reason?:
    | { $case: "ran"; ran: Empty }
    | { $case: "inferredFromTests"; inferredFromTests: InferredChain }
    | { $case: "inferredFromLogic"; inferredFromLogic: string }
    | { $case: "inferredThroughTestsOperation"; inferredThroughTestsOperation: InferredChainWithOperation }
    | undefined;
}

export interface Failed {
  reason?: { $case: "ran"; ran: Empty } | { $case: "inferredFromTests"; inferredFromTests: InferredChain } | {
    $case: "inferredThroughTestsOperation";
    inferredThroughTestsOperation: InferredChainWithOperation;
  } | undefined;
}

function createBaseTestResult(): TestResult {
  return { testName: "", query: "", testResult: undefined };
}

export const TestResult = {
  encode(message: TestResult, writer: _m0.Writer = _m0.Writer.create()): _m0.Writer {
    if (message.testName !== "") {
      writer.uint32(10).string(message.testName);
    }
    if (message.query !== "") {
      writer.uint32(18).string(message.query);
    }
    switch (message.testResult?.$case) {
      case "passed":
        Passed.encode(message.testResult.passed, writer.uint32(26).fork()).ldelim();
        break;
      case "failed":
        Failed.encode(message.testResult.failed, writer.uint32(34).fork()).ldelim();
        break;
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): TestResult {
    const reader = input instanceof _m0.Reader ? input : _m0.Reader.create(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseTestResult();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          if (tag !== 10) {
            break;
          }

          message.testName = reader.string();
          continue;
        case 2:
          if (tag !== 18) {
            break;
          }

          message.query = reader.string();
          continue;
        case 3:
          if (tag !== 26) {
            break;
          }

          message.testResult = { $case: "passed", passed: Passed.decode(reader, reader.uint32()) };
          continue;
        case 4:
          if (tag !== 34) {
            break;
          }

          message.testResult = { $case: "failed", failed: Failed.decode(reader, reader.uint32()) };
          continue;
      }
      if ((tag & 7) === 4 || tag === 0) {
        break;
      }
      reader.skipType(tag & 7);
    }
    return message;
  },

  fromJSON(object: any): TestResult {
    return {
      testName: isSet(object.testName) ? gt.String(object.testName) : "",
      query: isSet(object.query) ? gt.String(object.query) : "",
      testResult: isSet(object.passed)
        ? { $case: "passed", passed: Passed.fromJSON(object.passed) }
        : isSet(object.failed)
        ? { $case: "failed", failed: Failed.fromJSON(object.failed) }
        : undefined,
    };
  },

  toJSON(message: TestResult): unknown {
    const obj: any = {};
    if (message.testName !== "") {
      obj.testName = message.testName;
    }
    if (message.query !== "") {
      obj.query = message.query;
    }
    if (message.testResult?.$case === "passed") {
      obj.passed = Passed.toJSON(message.testResult.passed);
    }
    if (message.testResult?.$case === "failed") {
      obj.failed = Failed.toJSON(message.testResult.failed);
    }
    return obj;
  },

  create<I extends Exact<DeepPartial<TestResult>, I>>(base?: I): TestResult {
    return TestResult.fromPartial(base ?? ({} as any));
  },
  fromPartial<I extends Exact<DeepPartial<TestResult>, I>>(object: I): TestResult {
    const message = createBaseTestResult();
    message.testName = object.testName ?? "";
    message.query = object.query ?? "";
    if (
      object.testResult?.$case === "passed" &&
      object.testResult?.passed !== undefined &&
      object.testResult?.passed !== null
    ) {
      message.testResult = { $case: "passed", passed: Passed.fromPartial(object.testResult.passed) };
    }
    if (
      object.testResult?.$case === "failed" &&
      object.testResult?.failed !== undefined &&
      object.testResult?.failed !== null
    ) {
      message.testResult = { $case: "failed", failed: Failed.fromPartial(object.testResult.failed) };
    }
    return message;
  },
};

function createBaseInferredChain(): InferredChain {
  return { inferredChain: [] };
}

export const InferredChain = {
  encode(message: InferredChain, writer: _m0.Writer = _m0.Writer.create()): _m0.Writer {
    for (const v of message.inferredChain) {
      writer.uint32(10).string(v!);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): InferredChain {
    const reader = input instanceof _m0.Reader ? input : _m0.Reader.create(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseInferredChain();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          if (tag !== 10) {
            break;
          }

          message.inferredChain.push(reader.string());
          continue;
      }
      if ((tag & 7) === 4 || tag === 0) {
        break;
      }
      reader.skipType(tag & 7);
    }
    return message;
  },

  fromJSON(object: any): InferredChain {
    return {
      inferredChain: gt.Array.isArray(object?.inferredChain) ? object.inferredChain.map((e: any) => gt.String(e)) : [],
    };
  },

  toJSON(message: InferredChain): unknown {
    const obj: any = {};
    if (message.inferredChain?.length) {
      obj.inferredChain = message.inferredChain;
    }
    return obj;
  },

  create<I extends Exact<DeepPartial<InferredChain>, I>>(base?: I): InferredChain {
    return InferredChain.fromPartial(base ?? ({} as any));
  },
  fromPartial<I extends Exact<DeepPartial<InferredChain>, I>>(object: I): InferredChain {
    const message = createBaseInferredChain();
    message.inferredChain = object.inferredChain?.map((e) => e) || [];
    return message;
  },
};

function createBaseInferredChainWithOperation(): InferredChainWithOperation {
  return { inferredChain: [], operation: "" };
}

export const InferredChainWithOperation = {
  encode(message: InferredChainWithOperation, writer: _m0.Writer = _m0.Writer.create()): _m0.Writer {
    for (const v of message.inferredChain) {
      writer.uint32(10).string(v!);
    }
    if (message.operation !== "") {
      writer.uint32(18).string(message.operation);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): InferredChainWithOperation {
    const reader = input instanceof _m0.Reader ? input : _m0.Reader.create(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseInferredChainWithOperation();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          if (tag !== 10) {
            break;
          }

          message.inferredChain.push(reader.string());
          continue;
        case 2:
          if (tag !== 18) {
            break;
          }

          message.operation = reader.string();
          continue;
      }
      if ((tag & 7) === 4 || tag === 0) {
        break;
      }
      reader.skipType(tag & 7);
    }
    return message;
  },

  fromJSON(object: any): InferredChainWithOperation {
    return {
      inferredChain: gt.Array.isArray(object?.inferredChain) ? object.inferredChain.map((e: any) => gt.String(e)) : [],
      operation: isSet(object.operation) ? gt.String(object.operation) : "",
    };
  },

  toJSON(message: InferredChainWithOperation): unknown {
    const obj: any = {};
    if (message.inferredChain?.length) {
      obj.inferredChain = message.inferredChain;
    }
    if (message.operation !== "") {
      obj.operation = message.operation;
    }
    return obj;
  },

  create<I extends Exact<DeepPartial<InferredChainWithOperation>, I>>(base?: I): InferredChainWithOperation {
    return InferredChainWithOperation.fromPartial(base ?? ({} as any));
  },
  fromPartial<I extends Exact<DeepPartial<InferredChainWithOperation>, I>>(object: I): InferredChainWithOperation {
    const message = createBaseInferredChainWithOperation();
    message.inferredChain = object.inferredChain?.map((e) => e) || [];
    message.operation = object.operation ?? "";
    return message;
  },
};

function createBasePassed(): Passed {
  return { reason: undefined };
}

export const Passed = {
  encode(message: Passed, writer: _m0.Writer = _m0.Writer.create()): _m0.Writer {
    switch (message.reason?.$case) {
      case "ran":
        Empty.encode(message.reason.ran, writer.uint32(10).fork()).ldelim();
        break;
      case "inferredFromTests":
        InferredChain.encode(message.reason.inferredFromTests, writer.uint32(18).fork()).ldelim();
        break;
      case "inferredFromLogic":
        writer.uint32(26).string(message.reason.inferredFromLogic);
        break;
      case "inferredThroughTestsOperation":
        InferredChainWithOperation.encode(message.reason.inferredThroughTestsOperation, writer.uint32(34).fork())
          .ldelim();
        break;
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): Passed {
    const reader = input instanceof _m0.Reader ? input : _m0.Reader.create(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBasePassed();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          if (tag !== 10) {
            break;
          }

          message.reason = { $case: "ran", ran: Empty.decode(reader, reader.uint32()) };
          continue;
        case 2:
          if (tag !== 18) {
            break;
          }

          message.reason = {
            $case: "inferredFromTests",
            inferredFromTests: InferredChain.decode(reader, reader.uint32()),
          };
          continue;
        case 3:
          if (tag !== 26) {
            break;
          }

          message.reason = { $case: "inferredFromLogic", inferredFromLogic: reader.string() };
          continue;
        case 4:
          if (tag !== 34) {
            break;
          }

          message.reason = {
            $case: "inferredThroughTestsOperation",
            inferredThroughTestsOperation: InferredChainWithOperation.decode(reader, reader.uint32()),
          };
          continue;
      }
      if ((tag & 7) === 4 || tag === 0) {
        break;
      }
      reader.skipType(tag & 7);
    }
    return message;
  },

  fromJSON(object: any): Passed {
    return {
      reason: isSet(object.ran)
        ? { $case: "ran", ran: Empty.fromJSON(object.ran) }
        : isSet(object.inferredFromTests)
        ? { $case: "inferredFromTests", inferredFromTests: InferredChain.fromJSON(object.inferredFromTests) }
        : isSet(object.inferredFromLogic)
        ? { $case: "inferredFromLogic", inferredFromLogic: gt.String(object.inferredFromLogic) }
        : isSet(object.inferredThroughTestsOperation)
        ? {
          $case: "inferredThroughTestsOperation",
          inferredThroughTestsOperation: InferredChainWithOperation.fromJSON(object.inferredThroughTestsOperation),
        }
        : undefined,
    };
  },

  toJSON(message: Passed): unknown {
    const obj: any = {};
    if (message.reason?.$case === "ran") {
      obj.ran = Empty.toJSON(message.reason.ran);
    }
    if (message.reason?.$case === "inferredFromTests") {
      obj.inferredFromTests = InferredChain.toJSON(message.reason.inferredFromTests);
    }
    if (message.reason?.$case === "inferredFromLogic") {
      obj.inferredFromLogic = message.reason.inferredFromLogic;
    }
    if (message.reason?.$case === "inferredThroughTestsOperation") {
      obj.inferredThroughTestsOperation = InferredChainWithOperation.toJSON(
        message.reason.inferredThroughTestsOperation,
      );
    }
    return obj;
  },

  create<I extends Exact<DeepPartial<Passed>, I>>(base?: I): Passed {
    return Passed.fromPartial(base ?? ({} as any));
  },
  fromPartial<I extends Exact<DeepPartial<Passed>, I>>(object: I): Passed {
    const message = createBasePassed();
    if (object.reason?.$case === "ran" && object.reason?.ran !== undefined && object.reason?.ran !== null) {
      message.reason = { $case: "ran", ran: Empty.fromPartial(object.reason.ran) };
    }
    if (
      object.reason?.$case === "inferredFromTests" &&
      object.reason?.inferredFromTests !== undefined &&
      object.reason?.inferredFromTests !== null
    ) {
      message.reason = {
        $case: "inferredFromTests",
        inferredFromTests: InferredChain.fromPartial(object.reason.inferredFromTests),
      };
    }
    if (
      object.reason?.$case === "inferredFromLogic" &&
      object.reason?.inferredFromLogic !== undefined &&
      object.reason?.inferredFromLogic !== null
    ) {
      message.reason = { $case: "inferredFromLogic", inferredFromLogic: object.reason.inferredFromLogic };
    }
    if (
      object.reason?.$case === "inferredThroughTestsOperation" &&
      object.reason?.inferredThroughTestsOperation !== undefined &&
      object.reason?.inferredThroughTestsOperation !== null
    ) {
      message.reason = {
        $case: "inferredThroughTestsOperation",
        inferredThroughTestsOperation: InferredChainWithOperation.fromPartial(
          object.reason.inferredThroughTestsOperation,
        ),
      };
    }
    return message;
  },
};

function createBaseFailed(): Failed {
  return { reason: undefined };
}

export const Failed = {
  encode(message: Failed, writer: _m0.Writer = _m0.Writer.create()): _m0.Writer {
    switch (message.reason?.$case) {
      case "ran":
        Empty.encode(message.reason.ran, writer.uint32(10).fork()).ldelim();
        break;
      case "inferredFromTests":
        InferredChain.encode(message.reason.inferredFromTests, writer.uint32(18).fork()).ldelim();
        break;
      case "inferredThroughTestsOperation":
        InferredChainWithOperation.encode(message.reason.inferredThroughTestsOperation, writer.uint32(26).fork())
          .ldelim();
        break;
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): Failed {
    const reader = input instanceof _m0.Reader ? input : _m0.Reader.create(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseFailed();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          if (tag !== 10) {
            break;
          }

          message.reason = { $case: "ran", ran: Empty.decode(reader, reader.uint32()) };
          continue;
        case 2:
          if (tag !== 18) {
            break;
          }

          message.reason = {
            $case: "inferredFromTests",
            inferredFromTests: InferredChain.decode(reader, reader.uint32()),
          };
          continue;
        case 3:
          if (tag !== 26) {
            break;
          }

          message.reason = {
            $case: "inferredThroughTestsOperation",
            inferredThroughTestsOperation: InferredChainWithOperation.decode(reader, reader.uint32()),
          };
          continue;
      }
      if ((tag & 7) === 4 || tag === 0) {
        break;
      }
      reader.skipType(tag & 7);
    }
    return message;
  },

  fromJSON(object: any): Failed {
    return {
      reason: isSet(object.ran)
        ? { $case: "ran", ran: Empty.fromJSON(object.ran) }
        : isSet(object.inferredFromTests)
        ? { $case: "inferredFromTests", inferredFromTests: InferredChain.fromJSON(object.inferredFromTests) }
        : isSet(object.inferredThroughTestsOperation)
        ? {
          $case: "inferredThroughTestsOperation",
          inferredThroughTestsOperation: InferredChainWithOperation.fromJSON(object.inferredThroughTestsOperation),
        }
        : undefined,
    };
  },

  toJSON(message: Failed): unknown {
    const obj: any = {};
    if (message.reason?.$case === "ran") {
      obj.ran = Empty.toJSON(message.reason.ran);
    }
    if (message.reason?.$case === "inferredFromTests") {
      obj.inferredFromTests = InferredChain.toJSON(message.reason.inferredFromTests);
    }
    if (message.reason?.$case === "inferredThroughTestsOperation") {
      obj.inferredThroughTestsOperation = InferredChainWithOperation.toJSON(
        message.reason.inferredThroughTestsOperation,
      );
    }
    return obj;
  },

  create<I extends Exact<DeepPartial<Failed>, I>>(base?: I): Failed {
    return Failed.fromPartial(base ?? ({} as any));
  },
  fromPartial<I extends Exact<DeepPartial<Failed>, I>>(object: I): Failed {
    const message = createBaseFailed();
    if (object.reason?.$case === "ran" && object.reason?.ran !== undefined && object.reason?.ran !== null) {
      message.reason = { $case: "ran", ran: Empty.fromPartial(object.reason.ran) };
    }
    if (
      object.reason?.$case === "inferredFromTests" &&
      object.reason?.inferredFromTests !== undefined &&
      object.reason?.inferredFromTests !== null
    ) {
      message.reason = {
        $case: "inferredFromTests",
        inferredFromTests: InferredChain.fromPartial(object.reason.inferredFromTests),
      };
    }
    if (
      object.reason?.$case === "inferredThroughTestsOperation" &&
      object.reason?.inferredThroughTestsOperation !== undefined &&
      object.reason?.inferredThroughTestsOperation !== null
    ) {
      message.reason = {
        $case: "inferredThroughTestsOperation",
        inferredThroughTestsOperation: InferredChainWithOperation.fromPartial(
          object.reason.inferredThroughTestsOperation,
        ),
      };
    }
    return message;
  },
};

declare const self: any | undefined;
declare const window: any | undefined;
declare const global: any | undefined;
const gt: any = (() => {
  if (typeof globalThis !== "undefined") {
    return globalThis;
  }
  if (typeof self !== "undefined") {
    return self;
  }
  if (typeof window !== "undefined") {
    return window;
  }
  if (typeof global !== "undefined") {
    return global;
  }
  throw "Unable to locate global object";
})();

type Builtin = Date | Function | Uint8Array | string | number | boolean | undefined;

export type DeepPartial<T> = T extends Builtin ? T
  : T extends globalThis.Array<infer U> ? globalThis.Array<DeepPartial<U>>
  : T extends ReadonlyArray<infer U> ? ReadonlyArray<DeepPartial<U>>
  : T extends { $case: string } ? { [K in keyof Omit<T, "$case">]?: DeepPartial<T[K]> } & { $case: T["$case"] }
  : T extends {} ? { [K in keyof T]?: DeepPartial<T[K]> }
  : Partial<T>;

type KeysOfUnion<T> = T extends T ? keyof T : never;
export type Exact<P, I extends P> = P extends Builtin ? P
  : P & { [K in keyof P]: Exact<P[K], I[K]> } & { [K in Exclude<keyof I, KeysOfUnion<P>>]: never };

function isSet(value: any): boolean {
  return value !== null && value !== undefined;
}
