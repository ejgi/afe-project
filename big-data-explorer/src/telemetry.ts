import { createClient, SupabaseClient } from '@supabase/supabase-js';

// CONFIGURACIÓN GLOBAL DE TELEMETRÍA ZEN-ENGINE
// Estos datos conectan la extensión con el servidor central de telemetría.
const SUPABASE_URL = 'https://ewbtpmhbcelkosbwuqzd.supabase.co';
const SUPABASE_KEY = 'sb_publishable_uJhMXqNhEZTmsai9Dl1cAw_tQwrAUtX';

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
        if (SUPABASE_URL && SUPABASE_KEY) {
            this.client = createClient(SUPABASE_URL, SUPABASE_KEY);
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
