package com.tomcat.controller;

import com.tomcat.controller.requeset.ProfileReq;
import com.tomcat.controller.response.ProfileResp;
import com.tomcat.service.UserProfileService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping("/profile")
public class UserProfileController {

  @Autowired
  private UserProfileService userProfileService;

  @GetMapping("/getProfile")
  public ResponseEntity<ProfileResp> getByUserId(@RequestParam String userId) {
    ProfileResp profile = userProfileService.getByUserId(userId);
    return ResponseEntity.ok(profile); 
  }  

  @PutMapping("/update")
  public ResponseEntity<ProfileResp> updateProfile(@RequestBody ProfileReq profileDTO) {
    ProfileResp profile = userProfileService.update(profileDTO);
    return ResponseEntity.ok(profile);
  }

}