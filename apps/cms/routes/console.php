<?php

use App\Models\User;
use Illuminate\Support\Facades\Artisan;
use Illuminate\Support\Facades\Validator;
use Illuminate\Validation\Rules\Password;

use function Laravel\Prompts\password;

Artisan::command('cms:admin {email} {--name=Admin}', function (string $email) {
    $secret = password('Parola (en az 10 karakter)', required: true);
    $validator = Validator::make(['email' => $email, 'password' => $secret], [
        'email' => ['required', 'email'],
        'password' => [Password::defaults()],
    ]);
    if ($validator->fails()) {
        $this->error($validator->errors()->first());

        return 1;
    }
    $user = User::query()->updateOrCreate(['email' => $email], ['name' => $this->option('name'), 'password' => $secret]);
    $this->info(($user->wasRecentlyCreated ? 'Oluşturuldu: ' : 'Parolası güncellendi: ').$email);

    return 0;
})->purpose('Create an admin account or reset its password');
