# RKnowledge CLI Demo
Output assets/demo.gif
Set FontSize 14
Set Width 900
Set Height 500
Set Theme "Catppuccin Mocha"
Set TypingSpeed 40ms
Set Shell zsh

# Add rknowledge to PATH
Hide
Type "export PATH=$PATH:/Users/algimantask/Personal/RKnowledge/target/release"
Enter
Sleep 300ms
Type "cd /Users/algimantask/Personal/RKnowledge"
Enter
Sleep 300ms
Show

Type "# RKnowledge - Knowledge Graph CLI"
Enter
Sleep 500ms

Type "rknowledge --help"
Enter
Sleep 2s

Type "# Graph statistics with PageRank"
Enter
Sleep 300ms

Type "rknowledge stats"
Enter
Sleep 3s

Type "# Search for concepts"
Enter
Sleep 300ms

Type 'rknowledge query "neural networks" --depth 1'
Enter
Sleep 3s

Type "# Community detection"
Enter
Sleep 300ms

Type "rknowledge communities"
Enter
Sleep 3s

Type "# Export to JSON"
Enter
Sleep 300ms

Type "rknowledge export --output /tmp/graph.json --format json"
Enter
Sleep 2s

Type "# Open visualization"
Enter
Sleep 300ms

Type "rknowledge viz"
Enter
Sleep 2s

Type "# Done! 🚀"
Enter
Sleep 1s
