# Licensed under the Apache-2.0 license

from typing import TYPE_CHECKING

from peakrdl.plugins.exporter import ExporterSubcommandPlugin

if TYPE_CHECKING:
    import argparse
    from systemrdl.node import AddrmapNode

class Exporter(ExporterSubcommandPlugin):
    short_desc = "..."
    long_desc = "..."

    def add_exporter_arguments(self, arg_group: 'argparse.ArgumentParser') -> None:
        pass

    def do_export(self, top_node: 'AddrmapNode', options: 'argparse.Namespace') -> None:
        raise NotImplementedError
