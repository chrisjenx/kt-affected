package p

interface ListView {
    fun title(): String
}

class DefaultListView : Component(), ListView {
    init {
        events += label()
    }

    fun label(): String = "list"

    override fun title(): String = "List"
}
