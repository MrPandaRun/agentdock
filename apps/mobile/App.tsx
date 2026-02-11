import { StatusBar } from 'expo-status-bar';
import { StyleSheet, Text, View } from 'react-native';
import type { ProviderId } from "@agentdock/contracts/provider";

const supportedProviders: ProviderId[] = ["codex", "claude_code"];

export default function App() {
  return (
    <View style={styles.container}>
      <Text style={styles.title}>AgentDock Remote</Text>
      <Text>Provider support: {supportedProviders.join(', ')}</Text>
      <StatusBar style="auto" />
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#fff',
    alignItems: 'center',
    justifyContent: 'center',
  },
  title: {
    fontSize: 20,
    fontWeight: '600',
    marginBottom: 8,
  },
});
