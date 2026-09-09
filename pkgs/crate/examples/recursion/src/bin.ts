export function visitEvenLevel(node: Node) {
  console.log("Even:", node.value);

  for (const child of node.children) {
    visitOddLevel(child);
  }
}

export function visitOddLevel(node: Node) {
  console.log("Odd:", node.value);

  for (const child of node.children) {
    visitEvenLevel(child);
  }
}

interface Node {
  value: string;
  children: Node[];
}
