/*
 * Copyright 2016 WebAssembly Community Group participants
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#include <cassert>
#include <cinttypes>
#include <cstdio>
#include <cstdlib>
#include <iostream>

#include "wabt/apply-names.h"
#include "wabt/binary-reader.h"
#include "wabt/binary-reader-ir.h"
#include "wabt/error-formatter.h"
#include "wabt/feature.h"
#include "wabt/generate-names.h"
#include "wabt/ir.h"
#include "wabt/option-parser.h"
#include "wabt/resolve-names.h"
#include "wabt/stream.h"
#include "wabt/validator.h"
#include "wabt/wast-lexer.h"
#include "wabt/binary-writer.h"

#include "combine-modules.h"
#include "generate-prefix-names.h"
#include "resolve-imports.h"

using namespace wabt;

static int s_verbose;
static std::string s_infile;
static std::string s_lib_infile;
static std::string s_outfile = "";
static std::string s_infile_modname = "";
static std::string s_lib_infile_modname = "";
static Features s_features;
static bool s_resolve_names = true;
static bool s_read_debug_names = true;
static bool s_fail_on_custom_section_error = true;
static std::unique_ptr<FileStream> s_log_stream;
static bool s_validate = true;
static WriteBinaryOptions s_write_binary_options;

static const char s_description[] =
R"(  Read two files in the WebAssembly binary format, and convert it to
  the WebAssembly binary format, such that the output modules contain
  all fields of the two input wasm module, and imports in one input wasm
  file from the other wasm file are resolved as locals. File name must be
  the same as module name.

examples:
  # parse binary file moduleone.wasm and moduletwo.wasm write binary file output.wasm
  $ wasmlink moduleone.wasm moduletwo.wasm -o output.wasm
  # parse binary file moduleone.wasm with name env and moduletwo.wasm with name helper write binary file output.wasm
  $ wasmlink moduleone.wasm moduletwo.wasm -m env -n helper -o output.wasm
)";

static void ParseOptions(int argc, char** argv) {
  OptionParser parser("wasmlink", s_description);

  parser.AddOption('v', "verbose", "Use multiple times for more info", []() {
    s_verbose++;
    s_log_stream = FileStream::CreateStderr();
  });
  parser.AddOption(
      'o', "output", "FILENAME",
      "Output file for the generated wast file, by default use stdout",
      [](const char* argument) {
        s_outfile = argument;
        ConvertBackslashToSlash(&s_outfile);
      });
  parser.AddOption(
      'm', "first_mod_name", "FIRSTMODNAME",
      "Name of the first module",
      [](const char* argument) {
        s_infile_modname = argument;
      });
  parser.AddOption(
      'n', "second_mod_name", "SECONDMODNAME",
      "Name of the second module",
      [](const char* argument) {
        s_lib_infile_modname = argument;
      });
  s_features.AddOptions(&parser);
  parser.AddOption("no-debug-names", "Ignore debug names in the binary file",
                   []() { s_read_debug_names = false; });
  parser.AddOption("no-resolve-names", "Do not resolve names to index",
                   []() { s_resolve_names = false; });
  parser.AddOption("ignore-custom-section-errors",
                   "Ignore errors in custom sections",
                   []() { s_fail_on_custom_section_error = false; });
  parser.AddOption("no-check", "Don't check for invalid modules",
                   []() { s_validate = false; });
  parser.AddArgument("first_filename", OptionParser::ArgumentCount::One,
                     [](const char* argument) {
                       s_infile = argument;
                       ConvertBackslashToSlash(&s_infile);
                     });
  parser.AddArgument("second_filename", OptionParser::ArgumentCount::One,
                     [](const char* argument) {
                       s_lib_infile = argument;
                       ConvertBackslashToSlash(&s_lib_infile);
                     });
  parser.Parse(argc, argv);
}

std::string StripWasm(const std::string& file_name) {
  return file_name.substr(0, file_name.length() - 5);
}

int ProgramMain(int argc, char** argv) {
  Result result;
  Result result_lib;

  InitStdio();
  ParseOptions(argc, argv);

  std::vector<uint8_t> file_data;
  std::vector<uint8_t> lib_file_data;
  result = ReadFile(s_infile.c_str(), &file_data);
  result_lib = ReadFile(s_lib_infile.c_str(), &lib_file_data);
  if (Succeeded(result) && Succeeded(result_lib)) {
    Errors errors;
    Module module;
    Module libmodule;
    Module output;
    const bool kStopOnFirstError = true;
    ReadBinaryOptions options(s_features, s_log_stream.get(),
                              s_read_debug_names, kStopOnFirstError,
                              s_fail_on_custom_section_error);
    result = ReadBinaryIr(s_infile.c_str(), file_data.data(), file_data.size(),
                          options, &errors, &module);
    result_lib = ReadBinaryIr(s_lib_infile.c_str(), lib_file_data.data(), lib_file_data.size(),
                              options, &errors, &libmodule);
    module.name = !s_infile_modname.empty() ? s_infile_modname : StripWasm(s_infile);
    libmodule.name = !s_lib_infile_modname.empty() ? s_lib_infile_modname : StripWasm(s_lib_infile);
    if (Succeeded(result) && Succeeded(result_lib)) {
      if (s_validate) {
        ValidateOptions options(s_features);
        result = ValidateModule(&module, &errors, options);
        result_lib = ValidateModule(&libmodule, &errors, options);
      }

      if (Succeeded(result) && Succeeded(result_lib)) {
        result = GeneratePrefixNames(&module);
        result_lib = GeneratePrefixNames(&libmodule);
      }

      if (Succeeded(result) && Succeeded(result_lib)) {
        result = ApplyNames(&module);
        result_lib = ApplyNames(&libmodule);
      }

      if (Succeeded(result) && Succeeded(result_lib)) {
        std::unordered_map<std::string, std::string> import_map;
        result = ResolveImports(&module, &libmodule, &import_map);
      }

      if (Succeeded(result)) {
        result = CombineModules(&module, &libmodule, &output);
      }
      
      if (Succeeded(result) && s_resolve_names) {
        result = ResolveNamesModule(&output, &errors);
      }

      if (Succeeded(result) && s_validate) {
        ValidateOptions options(s_features);
        result = ValidateModule(&output, &errors, options);
      }

      if (Succeeded(result)) {
        MemoryStream stream;
        s_write_binary_options.features = s_features;
        result = WriteBinaryModule(&stream, &output, s_write_binary_options);
        stream.WriteToFile(s_outfile);
      }
    }
    FormatErrorsToFile(errors, Location::Type::Binary);
  }
  return result != Result::Ok || result_lib != Result::Ok;
}

int main(int argc, char** argv) {
  WABT_TRY
  return ProgramMain(argc, argv);
  WABT_CATCH_BAD_ALLOC_AND_EXIT
}
