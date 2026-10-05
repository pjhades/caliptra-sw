# Licensed under the Apache-2.0 license

class CaliptraExporter:
    def export(self, top_node: 'AddrmapNode', options: 'argparse.Namespace') -> str:
        for child in top_node.children(unroll=True):
            print(child.get_path())
