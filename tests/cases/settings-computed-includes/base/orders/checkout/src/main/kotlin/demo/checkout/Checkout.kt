package demo.checkout

import demo.catalog.Catalog

class Checkout(private val catalog: Catalog) {
    fun total(isbns: List<String>): Int = isbns.sumOf { catalog.price(it) }
}
