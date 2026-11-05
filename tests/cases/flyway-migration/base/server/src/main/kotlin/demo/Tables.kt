package demo

import org.jetbrains.exposed.sql.Table

object Horses : Table("horses") {
    val id = integer("id")
    val name = text("name")
}

object Riders : Table("riders") {
    val id = integer("id")
    val name = text("name")
}

class HorseDao {
    fun count(): Long = Horses.selectAll().count()
}

class RiderDao {
    fun count(): Long = Riders.selectAll().count()
}
