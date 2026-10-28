//#region Basic

function neverThrows() {
  return 1;
}

function throwSyncUnannotated() {
  throw new Error("Synchronous error");
}

function throwSync(): Returns<void, Error> {
  throw new Error("Synchronous error");
}

//#endregion

//#region Async

async function throwAsyncUnannotated() {
  throw new Error("Asynchronous error");
}

async function throwAsync(): Returns<Promise<void>, Error> {
  throw new Error("Asynchronous error");
}

//#endregion

//#region Union

class CustomError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "CustomError";
  }
}

function throwCustom(message: string): Returns<void, CustomError> {
  throw new CustomError(message);
}

function throwMixUnannotated(message: string) {
  if (Math.random() > 0.5) {
    throwSync();
  } else {
    throwCustom(message);
  }
}

function throwMixIncomplete(message: string): Returns<void, Error> {
  if (Math.random() > 0.5) {
    throwSync();
  } else {
    throwCustom(message);
  }
}

function throwMix(message: string): Returns<void, Error | CustomError> {
  if (Math.random() > 0.5) {
    throwSync();
  } else {
    throwCustom(message);
  }
}

//#endregion

//#region Throws

function throwMixInferred(
  message: string,
): Returns<void, Throws<typeof throwSync | typeof throwCustom>> {
  if (Math.random() > 0.5) {
    throwSync();
  } else {
    throwCustom(message);
  }
}

function withCallbackUnannotated(cb: () => void) {
  cb();
}

function withCallback(cb: () => void): Returns<void, Throws<typeof cb>> {
  cb();
}

//#endregion
