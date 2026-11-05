package demo

interface Repo {
    fun find(id: Int): String

    fun delete(id: Int)
}

class Other {
    fun noop() {}
}
