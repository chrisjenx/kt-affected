package demo.core

fun formatTotal(cents: Long): String = "${cents / 100}.${cents % 100}"

fun currency(): String = "EUR"
