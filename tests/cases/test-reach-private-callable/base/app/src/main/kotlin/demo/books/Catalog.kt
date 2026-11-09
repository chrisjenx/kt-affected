package demo.books

/** Its own private `render`, reached the same way. */
fun catalogLines(books: List<Book>): List<String> = books.map { it.render() }

private fun Book.render(): String = "- $title"

fun catalogHeader(): String = "Catalog"
