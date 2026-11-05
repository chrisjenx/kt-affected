package demo

expect fun platformName(short: Boolean = false): String

fun describe(): String = "running on " + platformName()

fun version(): Int = 3
