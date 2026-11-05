package demo

import demo.db.BookQueries
import demo.db.AuthorQueries

class BookStore(private val queries: BookQueries) {
    fun names(): List<String> = queries.selectAll().executeAsList().map { it.name }
}

class AuthorStore(private val queries: AuthorQueries) {
    fun named(n: String): Int = queries.authorsByName(n).executeAsList().size
}
