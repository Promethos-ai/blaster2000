import SwiftUI

struct ContentView: View {
    @State private var serverAddress = "192.168.1.100:4433"
    @State private var promptText = ""
    @State private var resultText = ""
    @State private var isConnecting = false

    var body: some View {
        VStack(spacing: 24) {
            Text("Ember AI")
                .font(.title)
                .fontWeight(.bold)

            TextField("Server (e.g. 192.168.1.100:4433 or ngrok URL)", text: $serverAddress)
                .textFieldStyle(.roundedBorder)
                .autocapitalization(.none)
                .textInputAutocapitalization(.never)

            TextField("Ask a question…", text: $promptText, axis: .vertical)
                .textFieldStyle(.roundedBorder)
                .lineLimit(3...6)

            Button(action: ask) {
                if isConnecting {
                    ProgressView()
                        .progressViewStyle(CircularProgressViewStyle())
                        .frame(maxWidth: .infinity)
                } else {
                    Text("Ask AI")
                        .frame(maxWidth: .infinity)
                }
            }
            .buttonStyle(.borderedProminent)
            .disabled(isConnecting || serverAddress.trimmingCharacters(in: .whitespaces).isEmpty || promptText.trimmingCharacters(in: .whitespaces).isEmpty)

            Text(resultText)
                .font(.body)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding()
                .background(Color(.systemGray6))
                .cornerRadius(8)
                .multilineTextAlignment(.leading)

            Spacer()
        }
        .padding(24)
    }

    private func ask() {
        let addr = serverAddress.trimmingCharacters(in: .whitespaces)
        let prompt = promptText.trimmingCharacters(in: .whitespaces)
        guard !addr.isEmpty, !prompt.isEmpty else { return }

        isConnecting = true
        resultText = "Asking AI..."

        DispatchQueue.global(qos: .userInitiated).async {
            let result = EmberClient.ask(serverAddr: addr, prompt: prompt)
            DispatchQueue.main.async {
                resultText = result
                isConnecting = false
            }
        }
    }
}

#Preview {
    ContentView()
}
