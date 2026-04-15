import { createClient, SupabaseClient } from '@supabase/supabase-js';
import * as vscode from 'vscode';

// CONFIGURACIÓN DE TELEMETRÍA ZEN-ENGINE
// Las credenciales se leen desde la configuración del usuario en VS Code.
// Si no están configuradas, la telemetría opera en modo local (solo consola).
function getSupabaseConfig(): { url: string; key: string } {
    const config = vscode.workspace.getConfiguration('zenExplorer');
    return {
        url: config.get<string>('supabaseUrl', ''),
        key: config.get<string>('supabaseKey', '')
    };
}

export interface TelemetryEvent {
    os: string;
    cpu: string;
    cores: number;
    ram_gb: number;
    file_size_mb: number;
    operation: string;
    duration_sec: number;
    throughput_gbs: number;
    disk_type: string;
    anonymous_id: string;
}

class TelemetryService {
    private client: SupabaseClient | null = null;
    private anonymousId: string = 'anon-' + Math.random().toString(36).substring(2, 15);

    constructor() {
        const { url, key } = getSupabaseConfig();
        if (url && key) {
            this.client = createClient(url, key);
        }
    }

    public async report(event: Omit<TelemetryEvent, 'anonymous_id'>) {
        if (!this.client) {
            console.log('Zen-Telemetry (Local-Only):', event);
            return;
        }

        try {
            const { error } = await this.client
                .from('zen_performance_logs')
                .insert([{ ...event, anonymous_id: this.anonymousId }]);

            if (error) {
                console.error('Zen-Telemetry Cloud Error:', error.message);
            } else {
                console.log('Zen-Telemetry Sync: Cloud-Success ✅');
            }
        } catch (e) {
            console.error('Zen-Telemetry Network Error:', e);
        }
    }
}

export const telemetry = new TelemetryService();
