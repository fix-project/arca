import json
import sys


def references(value, indices):
    if isinstance(value, list):
        return [references(child, indices) for child in value]
    if isinstance(value, dict):
        if list(value) == ["node"]:
            return {"node": indices[value["node"]]}
        return {key: references(child, indices) for key, child in value.items()}
    return value


with open(sys.argv[1]) as source, open(sys.argv[2], "w") as output:
    graph = json.load(source)
    indices = {}
    for name, value in graph["nodes"]:
        output.write(json.dumps(references(value, indices)) + "\n")
        indices[name] = len(indices)
    output.write(json.dumps(references(graph["result"], indices)) + "\n")
