package demo

import demo.db.HorseQueries
import demo.db.RiderQueries

class HorseStore(private val queries: HorseQueries) {
    fun names(): List<String> = queries.selectAll().executeAsList().map { it.name }
}

class RiderStore(private val queries: RiderQueries) {
    fun named(n: String): Int = queries.ridersByName(n).executeAsList().size
}
