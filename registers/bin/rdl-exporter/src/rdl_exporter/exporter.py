# Licensed under the Apache-2.0 license

# pub struct Scope {
#     pub ty: ScopeType,
#     pub types: HashMap<String, Scope>,
#     pub instances: Vec<Instance>,
#     pub default_properties: HashMap<String, Value>,
#     pub properties: HashMap<String, Value>,
#     pub dynamic_assignments: Vec<DynamicAssignment>,
# }

# pub enum Value {
#     U64(u64),
#     Bool(bool),
#     Bits(Bits),
#     String(String),
#     EnumReference(String),
#     Reference(Reference),
#     PrecedenceType(PrecedenceType),
#     AccessType(AccessType),
#     OnReadType(OnReadType),
#     OnWriteType(OnWriteType),
#     AddressingType(AddressingType),
#     InterruptType(InterruptType),
# }

class CaliptraExporter:
    def export(self, top_node: 'AddrmapNode', options: 'argparse.Namespace') -> str:
        properties = {}
        for name in top_node.list_properties():
            prop = top_node.get_property(name)
            properties[name] = prop
        #for child in top_node.children(unroll=True):
        #    print(child.get_path(), child.is_instance)
