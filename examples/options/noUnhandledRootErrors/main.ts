//#region Setup

function neverThrows() {
  return 1;
}

function throwSync() {
  throw new Error("Synchronous error");
}

async function throwAsync() {
  throw new Error("Asynchronous error");
}

class CustomError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "CustomError";
  }
}

function throwCustom(message: string) {
  throw new CustomError(message);
}

function throwMix(message: string) {
  if (Math.random() > 0.5) {
    throwSync();
  } else {
    throwCustom(message);
  }
}

//#endregion

//#region Basic

neverThrows();

throwSync();

try {
  throwSync();
} catch (error) {
  console.error(error);
}

//#endregion

//#region Rethrow

try {
  throwSync();
} catch {
  throwSync();
}

try {
  throwSync();
} catch {
  try {
    throwSync();
  } catch {}
}

//#endregion

//#region Promise

throwAsync();

throwAsync().catch((error) => {
  console.error(error);
});

//#endregion

//#region Promise Unhandled

throwAsync().catch(() => {
  throwAsync();
});

throwAsync().catch(() => {
  throwSync();
});

//#endregion

//#region

//#region Promise rethrow

throwAsync().catch(() => throwAsync());

throwAsync()
  .catch(() => throwAsync())
  .catch(() => {});

//#endregion

//#region Awaited

await throwAsync();

try {
  await throwAsync();
} catch (error) {
  console.error(error);
}

//#endregion

//#region Callback

function withCallback(cb: () => void) {
  cb();
}

withCallback(() => {});

withCallback(() => {
  throwSync();
});

//#endregion
