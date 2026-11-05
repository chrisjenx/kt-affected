package demo.legacy

import demo.catalog.Catalog

class Legacy(private val catalog: Catalog) {
    fun price(): Int = catalog.price("old")
}
