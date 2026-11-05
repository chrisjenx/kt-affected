package demo

import demo.db.HorseQueries
import demo.db.RiderQueries

class HorseStore(private val horses: HorseQueries) {
    fun names(): List<String> = horses.selectAll().executeAsList().map { it.name }
}

class RiderStore(private val riders: RiderQueries) {
    fun named(n: String): Int = riders.ridersByName(n).executeAsList().size
}
