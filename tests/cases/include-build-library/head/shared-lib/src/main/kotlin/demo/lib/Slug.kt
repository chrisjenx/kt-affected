package demo.lib

fun slug(s: String): String = s.trim().lowercase().replace(' ', '-')
