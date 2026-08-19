// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    armadillo_learn_desktop_lib::run()
}
// curl "https://generativelanguage.googleapis.com/v1beta/models/gemini-flash-latest:generateContent" \
//   -H 'Content-Type: application/json' \
//   -H 'X-goog-api-key: AQ.Ab8RN6I80ucLSQW2p7lhuBZqMlQvlESILxyQlnI5oyAOgeA5Tg' \
//   -X POST \
//   -d '{
//     "contents": [
//       {
//         "parts": [
//           {
//             "text": "Explain how AI works in a few words"
//           }
//         ]
//       }
//     ]
//   }'
// Gemini API Key
// AQ.Ab8RN6I80ucLSQW2p7lhuBZqMlQvlESILxyQlnI5oyAOgeA5Tg

// projects/36264479662
// 36264479662
// https://aistudio.google.com/docs/api-key