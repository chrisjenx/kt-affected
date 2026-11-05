package demo

expect fun platformName(): String

fun describe(): String = "running on " + platformName()

fun version(): Int = 3
