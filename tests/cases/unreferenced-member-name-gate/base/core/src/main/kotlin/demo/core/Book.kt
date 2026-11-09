package demo.core

class Book(val title: String) {
    fun display(): String = title.uppercase()

    fun shelfMark(): String = "B-" + title.length

    private fun neverCalled(): Int = 42
}

fun shelf(): List<Book> = listOf(Book("Dune"), Book("Emma"))
