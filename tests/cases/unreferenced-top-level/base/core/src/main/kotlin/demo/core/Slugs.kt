package demo.core

fun slug(title: String): String = title.lowercase().replace(' ', '-')

fun legacySlug(title: String): String = title.replace(' ', '_')

class Unused

object Catalog {
    const val NAME = "books"
}
