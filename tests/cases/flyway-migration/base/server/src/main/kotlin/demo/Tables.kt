package demo

import org.jetbrains.exposed.sql.Table

object Books : Table("books") {
    val id = integer("id")
    val name = text("name")
}

object Authors : Table("authors") {
    val id = integer("id")
    val name = text("name")
}

class BookDao {
    fun count(): Long = Books.selectAll().count()
}

class AuthorDao {
    fun count(): Long = Authors.selectAll().count()
}
