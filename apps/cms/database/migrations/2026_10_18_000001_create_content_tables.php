<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

/**
 * Website content. Every page (categories, product pages, posts, docs) is a row of `pages`, a
 * tree through `parent_id`; its texts live in one `page_translations` row per language.
 */
return new class extends Migration
{
    public function up(): void
    {
        Schema::create('languages', function (Blueprint $table) {
            $table->id();
            // URL prefix and hreflang value: `en`, `tr`, `pt-br`.
            $table->string('code', 12)->unique();
            $table->string('name');
            $table->string('native_name');
            // BCP 47 tag for <html lang>, Open Graph and number formats: `en-US`.
            $table->string('locale_tag', 20);
            $table->boolean('is_default')->default(false);
            $table->boolean('is_active')->default(true);
            $table->unsignedInteger('position')->default(0);
            $table->timestamps();
        });

        Schema::create('media', function (Blueprint $table) {
            $table->id();
            $table->string('disk', 40);
            $table->string('path')->unique();
            $table->string('filename');
            $table->string('mime', 100);
            $table->unsignedBigInteger('size');
            $table->unsignedInteger('width')->nullable();
            $table->unsignedInteger('height')->nullable();
            $table->string('alt')->nullable();
            $table->timestamps();
        });

        Schema::create('pages', function (Blueprint $table) {
            $table->id();
            $table->foreignId('parent_id')->nullable()->constrained('pages')->restrictOnDelete();
            $table->string('type', 20)->index();
            // Program the page presents (config cms.products), for downloads and product cards.
            $table->string('product', 40)->nullable()->index();
            $table->string('icon', 40)->nullable();
            $table->unsignedInteger('position')->default(0);
            $table->boolean('in_header')->default(false);
            $table->string('footer_column', 20)->nullable();
            $table->timestamps();
        });

        Schema::create('page_translations', function (Blueprint $table) {
            $table->id();
            $table->foreignId('page_id')->constrained()->cascadeOnDelete();
            $table->string('locale', 12);
            $table->foreign('locale')->references('code')->on('languages')->cascadeOnUpdate()->restrictOnDelete();
            // Last URL segment; empty for the home page.
            $table->string('slug', 160);
            // Slugs of the ancestors in this language and this page's, joined with `/`.
            $table->string('path', 400);
            $table->string('title');
            $table->string('nav_title')->nullable();
            $table->text('excerpt')->nullable();
            $table->json('blocks');
            $table->string('seo_title')->nullable();
            $table->string('seo_description', 320)->nullable();
            $table->string('canonical_url')->nullable();
            $table->boolean('noindex')->default(false);
            $table->foreignId('image_id')->nullable()->constrained('media')->nullOnDelete();
            $table->string('status', 12)->default('draft')->index();
            $table->timestamp('published_at')->nullable();
            $table->foreignId('updated_by')->nullable()->constrained('users')->nullOnDelete();
            $table->timestamps();

            $table->unique(['page_id', 'locale']);
            $table->unique(['locale', 'path']);
        });

        Schema::create('redirects', function (Blueprint $table) {
            $table->id();
            // Full old address with the language prefix: `/tr/features`.
            $table->string('from_path', 460)->unique();
            // A redirect follows its translation wherever it moves; manual ones name a target.
            $table->foreignId('translation_id')->nullable()->constrained('page_translations')->cascadeOnDelete();
            $table->string('target')->nullable();
            $table->unsignedSmallInteger('status_code')->default(301);
            $table->boolean('automatic')->default(false);
            $table->unsignedInteger('hits')->default(0);
            $table->timestamp('last_hit_at')->nullable();
            $table->timestamps();
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('redirects');
        Schema::dropIfExists('page_translations');
        Schema::dropIfExists('pages');
        Schema::dropIfExists('media');
        Schema::dropIfExists('languages');
    }
};
