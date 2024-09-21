package screenshare.client.service

import dev.gitlive.firebase.Firebase
import dev.gitlive.firebase.FirebaseOptions
import dev.gitlive.firebase.firestore.DocumentReference
import dev.gitlive.firebase.firestore.FirebaseFirestore
import dev.gitlive.firebase.firestore.firestore
import dev.gitlive.firebase.initialize

class FirebaseService(
    private val database: FirebaseFirestore,
) {
    suspend fun createCallDoc(): DocumentReference {
        val collection = database.collection("calls")
        return collection.add(mapOf("defaultData" to "defaultData")) { this.encodeDefaults = true }
    }

    fun getCallDoc(callId: String): DocumentReference {
        return database.collection("calls").document(callId)
    }

    companion object {
        fun create(config: FirebaseOptions): FirebaseService {
            val firebaseApp = Firebase.initialize(null, config)
            return FirebaseService(
                database = Firebase.firestore(firebaseApp),
            )
        }
    }
}

fun getFirebaseConfig(): FirebaseOptions {
    return FirebaseOptions(
        apiKey = "",
        authDomain = "",
        projectId = "",
        storageBucket = "",
        applicationId = "",
        gcmSenderId = "",
    )
}
