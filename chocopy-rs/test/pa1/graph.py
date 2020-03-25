class weight(object):
    value: int = 0

    def __init__(self: "weight", value: int):
        self.value = value

    def __repr__(self: "weight"):
        return "weight(" + str(self.value) + ")"


class graph(object):
    matrix: [[weight]] = None

    def __init__(self: "graph", num_vertices: int):
        i: int = 0
        j: int = 0
        row: [weight] = None

        self.matrix = []
        while i < num_vertices:
            j = 0
            row = []
            while j < num_vertices:
                row = row + [weight(0)]
                j = j + 1
            self.matrix = self.matrix + [row]
            i = i + 1

    def add_edge(self: "graph", from_vertex: int, to_vertex: int, edge_weight: int):
        pass

    def print(self: "graph"):
        for row in self.matrix:
            print(row)


class undirectedgraph(graph):
    def add_edge(self: "undirectedgraph", from_vertex: int, to_vertex: int, edge_weight: int):
        self.matrix[from_vertex][to_vertex] = weight(edge_weight)
        self.matrix[to_vertex][from_vertex] = weight(edge_weight)


class directedgraph(graph):
    def add_edge(self: "undirectedgraph", from_vertex: int, to_vertex: int, edge_weight: int):
        self.matrix[from_vertex][to_vertex] = weight(edge_weight)


g = undirectedgraph(3)
g.add_edge(0, 2, 100)
g.print()

directedgraph(3).print()
