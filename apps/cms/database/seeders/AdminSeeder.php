<?php

namespace Database\Seeders;

use App\Models\User;
use Illuminate\Database\Seeder;
use Illuminate\Support\Str;

/** The first admin account (config cms.admin); skipped when any account exists. */
class AdminSeeder extends Seeder
{
    public function run(): void
    {
        if (User::query()->exists()) {
            return;
        }
        $password = (string) config('cms.admin.password');
        $generated = $password === '';
        if ($generated) {
            $password = Str::password(16, symbols: false);
        }
        User::query()->create([
            'name' => config('cms.admin.name'),
            'email' => config('cms.admin.email'),
            'password' => $password,
        ]);
        $this->command?->info('Admin: '.config('cms.admin.email').($generated ? "  parola: $password  (bir kez gösterilir)" : ''));
    }
}
