//#region Setup

class CustomError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "CustomError";
  }
}

function throwMixSync(message: string) {
  if (Math.random() > 0.5) {
    throw new Error();
  } else if (Math.random() > 0.5) {
    throw new CustomError(message);
  }
  throw "NOPE"
}

async function throwMixAsync(message: string) {
  return throwMixSync(message);
}


//#endregion

//#region Catch

try {
  throwMixSync("Test message");
} catch (error) {
  if (error instanceof CustomError) {
    console.error("Caught a CustomError:", error.message);
    error satisfies CustomError;
  } else if (error instanceof Error) {
    console.error("Caught an Error:", error.message);
    error satisfies Error;
  } else {
    console.error('Caught the string "NOPE"');
    error satisfies "NOPE";
  }
}

//#endregion

//#region Promise

throwMixAsync("Test message").catch((error) => {
  if (error instanceof CustomError) {
    console.error("Caught a CustomError:", error.message);
    error satisfies CustomError;
  } else if (error instanceof Error) {
    console.error("Caught an Error:", error.message);
    error satisfies Error;
  } else {
    console.error('Caught the string "NOPE"');
    error satisfies "NOPE";
  }
});


//#endregion





















