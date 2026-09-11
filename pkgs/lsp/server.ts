import { startServer } from "./dist/index.js";

startServer().then(
  () => process.exit(0),
  (error) => {
    console.error(error);
    process.exit(1);
  },
);
