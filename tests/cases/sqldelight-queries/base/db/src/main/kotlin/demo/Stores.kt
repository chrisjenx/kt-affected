package demo

import demo.db.BookQueries
import demo.db.AuthorQueries

class BookStore(private val books: BookQueries) {
    fun names(): List<String> = books.selectAll().executeAsList().map { it.name }
}

class AuthorStore(private val authors: AuthorQueries) {
    fun named(n: String): Int = authors.authorsByName(n).executeAsList().size
}
