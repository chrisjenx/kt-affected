package demo.shelves

enum class Route { LIST, CREATE, DELETE }

abstract class Element(val key: Any)

class Slot : Element(Slot) {
    companion object Key
}

fun slot(): Slot = Slot()
