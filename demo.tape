# RKnowledge CLI Demo
Output assets/demo.gif
Set FontSize 14
Set Width 900
Set Height 500
Set Theme "Catppuccin Mocha"
Set TypingSpeed 40ms

Type "# RKnowledge - Knowledge Graph CLI"
Enter
Sleep 500ms

Type "./target/release/rknowledge --help"
Enter
Sleep 2s

Type "# Graph statistics with PageRank"
Enter
Sleep 300ms

Type "./target/release/rknowledge stats"
Enter
Sleep 3s

Type "# Search for concepts"
Enter
Sleep 300ms

Type './target/release/rknowledge query "neural networks" --depth 1'
Enter
Sleep 3s

Type "# Community detection"
Enter
Sleep 300ms

Type "./target/release/rknowledge communities"
Enter
Sleep 3s

Type "# Export to JSON"
Enter
Sleep 300ms

Type "./target/release/rknowledge export --output /tmp/graph.json --format json"
Enter
Sleep 2s

Type "# Open visualization"
Enter
Sleep 300ms

Type "./target/release/rknowledge viz"
Enter
Sleep 2s

Type "# Done! 🚀"
Enter
Sleep 1s
