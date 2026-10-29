//#region Setup

async function neverThrowsAsync() {
  return 1;
}

async function throwAsync() {
  throw new Error("Asynchronous error");
}

//#endregion

//#region Basic

function main() {
  neverThrowsAsync();

  throwAsync();

  throwAsync().catch((error) => {
    console.error(error);
  });

  throwAsync().catch((error) => {
    throwAsync();
  });
}

//#endregion
