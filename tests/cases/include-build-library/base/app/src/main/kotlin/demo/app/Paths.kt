package demo.app

import demo.lib.slug

fun pathFor(title: String): String = "/" + slug(title)
