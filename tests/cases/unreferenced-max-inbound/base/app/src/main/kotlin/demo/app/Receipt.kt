package demo.app

import demo.core.currency
import demo.core.formatTotal

fun receipt(cents: Long): String = formatTotal(cents) + " " + currency()

fun header(): String = currency()

fun main() {
    println(receipt(1250) + header())
}
