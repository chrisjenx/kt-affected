package demo.books

/** Writes `render` through an untyped `it`, with no `render` of its own in view. */
fun shelfTitles(books: List<Book>): List<String> = books.map { it.render() }

fun Book.render(): String = title
