package com.tomcat.controller.response;

import com.unfbx.chatgpt.entity.common.Usage;
import lombok.Data;

import java.util.List;

/**
 * @projectName: Tomcat
 * @package: com.tomcat.controller.response
 * @className: TipsResp
 * @author: tomcat
 * @description: TODO
 * @date: 2023/9/7 4:04 PM
 * @version: 1.0
 */

@Data
public class TipsResp {
    private List<String> tips;

}
