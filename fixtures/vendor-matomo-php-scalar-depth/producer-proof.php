<?php
$source = $argv[1] ?? '/evidence/Common.php';
if (hash_file('sha256', $source) !== 'b7bac3bbbea3eaa743f521b49b320abda6425923e4d4ff1331e2eb21bfeed79e') {
    throw new Exception('Matomo source differs from pinned Common.php');
}
require $source;
$queries = [
'idsite=0&idsite=1&rec=1',
'idsite=1&idsite=0&rec=1',
'idsite=1&rec=no&rec=1',
'idsite=1&rec=1&rec=no',
'idsite=1&rec=1&e.c=Video',
'idsite=1&rec=1&e+c=Video&e_a=play',
'idsite=1&rec=1&e.c=Old&e_c=New&e_a=play',
'%20%20idsite=1&rec=1',
'idsite=+1%20&rec=1.0',
'idsite=1e0&rec=01',
'idsite=1.5&rec=1',
'idsite=1&rec=2e-1',
'idsite=1&rec=1&e_v=1%2C5',
'idsite=1&rec=1&url=https%3A%2F%2Fexample.test%2F%3Fq%3Da%2Bb',
'idsite=1&rec=1&e_c=hello%2520world&e_a=a%2Bb',
'idsite[]=1&rec=1',
'idsite=1&rec[]=1',
'idsite[site]=1&rec=1',
'idsite=0&idsite[]=1&idsite=2&rec=1',
'idsite=1&rec=1&e.c[]=Video&e_a=play',
'idsite%00ignored=1&rec=1',
'idsite=1&rec=1&uid=a%00b',
'idsite=%26%2349%3B&rec=1',
'idsite=1;rec=1',
'idsite=1&rec=1&=ignored&%20=ignored',
'idsite=1&rec=1&&e_c=Video&e_a=play',
'idsite=1&rec=1&uid=%FF',
];
$rows = [];
foreach ($queries as $query) {
  parse_str($query, $params);
  $readers = [];
  foreach (['idsite'=>[0,'integer'],'rec'=>[0,'integer'],'e_c'=>['','string'],'e_a'=>['','string'],
    'e_v'=>[false,'float'],'url'=>['','string'],'uid'=>['','string']] as $name=>$spec) {
    $readers[$name] = \Piwik\Common::getRequestVar($name,$spec[0],$spec[1],$params);
  }
  $rows[]=['query'=>$query,'parsed'=>$params,'readers'=>$readers];
}
$floatRows = [];
foreach (['1,5','-1,5',',5','-,5','+,5','0,5','-0,5','1,','1,2,3','1,5e3',
    '1.5','-1.5','1.50','.5','-.5','1e2','2147483647,5','2147483648,5',
    '-2147483648,5','-2147483649,5','not numeric', '', true, false, 1, 0.1, ['opaque']] as $input) {
    try {
        $output = \Piwik\Common::getRequestVar('e_v',false,'float',['e_v'=>$input]);
        $floatRows[] = ['input'=>$input,'output'=>$output];
    } catch (\Throwable $error) {
        $floatRows[] = ['input'=>$input,'exception_class'=>get_class($error)];
    }
}
echo json_encode(['php_version'=>PHP_VERSION,'php_int_size'=>PHP_INT_SIZE,
  'arg_separator_input'=>ini_get('arg_separator.input'),'max_input_vars'=>ini_get('max_input_vars'),
  'max_input_nesting_level'=>ini_get('max_input_nesting_level'),'precision'=>ini_get('precision'),
  'source_sha256'=>hash_file('sha256',$source),'rows'=>$rows,'native_float_rows'=>$floatRows],
 JSON_PRETTY_PRINT|JSON_INVALID_UTF8_SUBSTITUTE|JSON_UNESCAPED_SLASHES)."\n";
