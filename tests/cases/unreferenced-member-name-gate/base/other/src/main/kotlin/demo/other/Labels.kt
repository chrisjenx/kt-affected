package demo.other

// Writes `shelfMark`, but cannot see :core: not a use of Book.shelfMark.
class Labels {
    fun shelfMark(): String = "label"

    fun print(): String = shelfMark()
}
