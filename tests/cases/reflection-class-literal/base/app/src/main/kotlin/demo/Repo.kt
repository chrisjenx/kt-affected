package demo

interface Repo {
    fun find(id: Int): String
}

class Other {
    fun noop() {}
}
