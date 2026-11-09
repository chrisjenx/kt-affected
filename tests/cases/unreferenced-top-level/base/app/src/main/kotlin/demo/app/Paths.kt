package demo.app

import demo.core.Catalog
import demo.core.slug

fun bookPath(title: String): String = "/${Catalog.NAME}/" + slug(title)

fun main() {
    println(bookPath("Dune"))
}
