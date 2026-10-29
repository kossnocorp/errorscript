const expected = process.env.ERRS_LIB_NODE_VERSION ?? "24";
const actual = process.versions.node.split(".")[0];
if (actual !== expected) {
  throw new Error(
    `Node ${expected} inventory requires Node ${expected}.x; running ${process.version}`,
  );
}
