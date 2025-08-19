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
        apiKey = "AIzaSyBKNGr5tEqUpnICruHaThEWimjT_yZ89Ho",
        authDomain = "screenshare-test-90191.firebaseapp.com",
        projectId = "screenshare-test-90191",
        storageBucket = "screenshare-test-90191.appspot.com",
        applicationId = "1:143281145249:web:98f04465b6156cf46837e0",
        gcmSenderId = "143281145249",
    )
}
