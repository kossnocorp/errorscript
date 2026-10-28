//#region Setup

async function neverThrows() {
  return 1;
}

async function throwAsync() {
  throw new Error("Asynchronous error");
}

//#endregion

//#region Basic

function main() {
  neverThrows();

  throwAsync();

  throwAsync().catch((error) => {
    console.error(error);
  });

  throwAsync().catch((error) => {
    throwAsync();
  });
}

//#endregion
