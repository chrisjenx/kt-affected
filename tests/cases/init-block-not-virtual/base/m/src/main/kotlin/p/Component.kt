package p

abstract class Component(private val track: Boolean = true) {
    val events = mutableListOf<String>()

    protected val scope = Scope()

    init {
        if (track) events += "created"
    }

    abstract fun title(): String
}

class Scope {
    fun label(text: String): String = "[$text]"
}
